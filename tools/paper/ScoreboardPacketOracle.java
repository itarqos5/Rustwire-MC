// Original API-only wire probe; generates test inputs and calls installed packet deserializers.
import java.lang.reflect.*;
import java.util.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
public class ScoreboardPacketOracle {
    static Class<?> cls(String...n)throws Exception {
        return ScoreboardDomainOracle.cls(n);
    }
    static int protocol;
    static final java.io.PrintStream OUT=System.out;
    static class Wire extends ByteArrayOutputStream {
        Wire b(int v) {
            write(v);
            return this;
        }
        Wire var(int v) {
            while((v&~127)!=0) {
                b((v&127)|128);
                v>>>=7;
            }
            return b(v);
        }
        Wire utf(String s) {
            byte[] a=s.getBytes(StandardCharsets.UTF_8);
            var(a.length);
            writeBytes(a);
            return this;
        }
        Wire text(String s) {
            if(protocol<765)return utf("\""+s+"\"");
            b(8).b((s.length()>>8)&255).b(s.length()&255);
            writeBytes(s.getBytes(StandardCharsets.UTF_8));
            return this;
        }
    }
    static Object buf(Class<?> b,byte[] payload)throws Exception {
        Class<?> bb=cls("io.netty.buffer.ByteBuf");
        Object raw=cls("io.netty.buffer.Unpooled").getMethod("wrappedBuffer",byte[].class).invoke(null,(Object)payload);
        for(Constructor<?> c:b.getConstructors()) {
            Class<?>[] ts=c.getParameterTypes();
            if(ts.length>0&&ts[0]==bb) {
                Object[] a=new Object[ts.length];
                a[0]=raw;
                for(int i=1;i<ts.length;i++) {
                    if(ts[i]==boolean.class)a[i]=false;
                    else if(ts[i].getName().equals("net.minecraft.core.RegistryAccess")) {
                        Class<?> br=cls("net.minecraft.core.registries.BuiltInRegistries");
                        Object registries=br.getField("REGISTRY").get(null);
                        for(Method m:ts[i].getMethods())if(m.getName().equals("fromRegistryOfRegistries"))a[i]=m.invoke(null,registries);
                    }
                    else {
                        for(Field f:ts[i].getFields())if(Modifier.isStatic(f.getModifiers())&&ts[i].isAssignableFrom(f.getType())) {
                            a[i]=f.get(null);
                            break;
                        }
                    }
                }
                return c.newInstance(a);
            }
        }
        throw new IllegalArgumentException("no buffer "+b);
    }
    static String value(Object x)throws Exception {
        if(x==null)return "null";
        if(x instanceof String s)return s.length()>50?"String[len="+s.length()+"]":'"'+s+'"';
        if(x instanceof Enum<?>e)return e.name();
        if(x instanceof Optional<?>o)return o.isEmpty()?"empty":value(o.get());
        if(x instanceof Number||x instanceof Boolean)return x.toString();
        if(x instanceof Collection<?>c)return c.toString();
        List<String> a=new ArrayList<>();
        for(Field f:x.getClass().getDeclaredFields()) {
            if(Modifier.isStatic(f.getModifiers()))continue;
            f.setAccessible(true);
            Object z=f.get(x);
            a.add(f.getName()+"="+(z instanceof String||z instanceof Enum||z instanceof Number||z instanceof Optional?value(z):z==null?"null":z.getClass().getSimpleName()));
        }
        return x.getClass().getSimpleName()+a;
    }
    static void decode(String name,byte[] payload,Class<?> c) {
        try {
            Object o=null;
            Class<?> b=null;
            for(Constructor<?> k:c.getDeclaredConstructors()) {
                if(k.getParameterCount()==1&&k.getParameterTypes()[0].getName().contains("ByteBuf")||k.getParameterCount()==1&&k.getParameterTypes()[0].getName().equals("net.minecraft.network.PacketDataSerializer")) {
                    b=k.getParameterTypes()[0];
                    k.setAccessible(true);
                    o=k.newInstance(buf(b,payload));
                    break;
                }
            }
            if(o==null) {
                Field f=c.getField("STREAM_CODEC");
                Object codec=f.get(null);
                Class<?> iface=cls("net.minecraft.network.codec.StreamCodec");
                b=cls("net.minecraft.network.RegistryFriendlyByteBuf");
                o=iface.getMethod("decode",Object.class).invoke(codec,buf(b,payload));
            }
            OUT.println(name+" "+value(o));
        }
        catch(Throwable e) {
            OUT.println(name+" ERROR "+ScoreboardDomainOracle.err(e));
        }
    }
    static Class<?> packet(String newer,String older)throws Exception {
        return cls("net.minecraft.network.protocol.game."+newer,"net.minecraft.network.protocol.game."+older);
    }
    public static void main(String[] args)throws Exception {
        protocol=Integer.parseInt(args[0]);
        Class<?> sc=cls("net.minecraft.SharedConstants");
        sc.getMethod(protocol<766?"a":"tryDetectVersion").invoke(null);
        Class<?> boot=cls("net.minecraft.server.Bootstrap","net.minecraft.server.DispenserRegistry");
        boot.getMethod(protocol<766?"a":"bootStrap").invoke(null);
        Class<?> display=packet("ClientboundSetDisplayObjectivePacket","PacketPlayOutScoreboardDisplayObjective");
        for(int id:new int[] {
            -128,-1,0,18,19,127,128
        }
        )decode("display_slot_"+id, (protocol==763?new Wire().b(id):new Wire().var(id)).utf("").toByteArray(),display);
        Class<?> obj=packet("ClientboundSetObjectivePacket","PacketPlayOutScoreboardObjective");
        for(int action:new int[] {
            -128,-1,1,3,127
        }
        )decode("objective_action_"+action,new Wire().utf("O").b(action).toByteArray(),obj);
        Class<?> team=packet("ClientboundSetPlayerTeamPacket","PacketPlayOutScoreboardTeam");
        for(int mode:new int[] {
            -128,-1,1,5,127
        }
        )decode("team_mode_"+mode,new Wire().utf("T").b(mode).toByteArray(),team);
        Class<?> param=packet("ClientboundSetPlayerTeamPacket$Parameters","PacketPlayOutScoreboardTeam$b");
        for(int color:new int[] {
            -1,0,15,16,20,21,22
        }
        ) {
            Wire w=new Wire().text("D");
            if(protocol==776)w.text("P").text("S").var(7).var(-1).b(1).var(color).b(255);
            else {
                w.b(255);
                if(protocol<770)w.utf("unknownVisibility").utf("unknownCollision");
                else w.var(7).var(-1);
                w.var(color).text("P").text("S");
            }
            decode("team_params_color_"+color,w.toByteArray(),param);
        }
        if(protocol<770)for(int n:new int[] {
            40,41
        }
        )decode("visibility_length_"+n,new Wire().text("D").b(3).utf("x".repeat(n)).utf("always").var(21).text("").text("").toByteArray(),param);
        for(int n:new int[] {
            41,32767,32768
        }
        ) {
            decode("objective_name_len_"+n,new Wire().utf("x".repeat(n)).b(1).toByteArray(),obj);
            decode("team_name_len_"+n,new Wire().utf("x".repeat(n)).b(1).toByteArray(),team);
        }
        Class<?> score=packet("ClientboundSetScorePacket","PacketPlayOutScoreboardScore");
        if(protocol<765) {
            for(int action:new int[] {
                -1,0,1,2
            }
            ) {
                Wire w=new Wire().utf("H").var(action).utf("");
                if(action!=1)w.var(Integer.MIN_VALUE);
                decode("score_action_"+action,w.toByteArray(),score);
            }
        }
        else {
            for(int n:new int[] {
                Integer.MIN_VALUE,-1,0,Integer.MAX_VALUE
            }
            )decode("score_value_"+n,new Wire().utf("H").utf("O").var(n).b(0).b(0).toByteArray(),score);
            Class<?> reset=packet("ClientboundResetScorePacket","ClientboundResetScorePacket");
            decode("reset_no_objective",new Wire().utf("H").b(0).toByteArray(),reset);
            decode("reset_empty_objective",new Wire().utf("H").b(1).utf("").toByteArray(),reset);
        }
        for(int render:new int[] {
            -1,0,1,2
        }
        ) {
            Wire w=new Wire().utf("O").b(0).text("D").var(render);
            if(protocol>=765)w.b(0);
            decode("render_"+render,w.toByteArray(),obj);
        }
        for(int n:new int[] {
            41,32767,32768
        }
        ) {
            Wire w=new Wire().utf("x".repeat(n));
            if(protocol<765)w.var(1).utf("O");
            else w.utf("O").var(0).b(0).b(0);
            decode("score_owner_len_"+n,w.toByteArray(),score);
            decode("team_player_len_"+n,new Wire().utf("T").b(3).var(1).utf("x".repeat(n)).toByteArray(),team);
        }
        if(protocol>=765) {
            for(int format:new int[] {
                0,1,2,3
            }
            ) {
                Wire w=new Wire().utf("O").b(0).text("D").var(0).b(1).var(format);
                if(format==1)w.b(10).b(0);
                if(format==2)w.text("X");
                decode("objective_format_"+format,w.toByteArray(),obj);
            }
            for(int root:new int[] {
                0,1,8,9,10
            }
            ) {
                Wire w=new Wire().utf("O").b(0).text("D").var(0).b(1).var(1).b(root);
                if(root==1)w.b(1);
                if(root==8)w.b(0).b(1).b('X');
                if(root==9)w.b(0).b(0).b(0).b(0).b(0);
                if(root==10)w.b(0);
                decode("style_root_"+root,w.toByteArray(),obj);
            }
        }
    }
}
