// Original read-only API probe. No Minecraft implementation is reproduced.
import java.lang.reflect.*;
import java.util.*;
import java.util.function.*;
import java.io.*;
import java.nio.charset.StandardCharsets;
public class ScoreboardDomainOracle {
    static Class<?> cls(String... names) throws Exception {
        for(String n:names)try {
            return Class.forName(n);
        }
        catch(ClassNotFoundException e) {
        }
        throw new ClassNotFoundException(Arrays.toString(names));
    }
    static Method exact(Class<?> c,Class<?> r,Class<?>... p) {
        for(Method m:c.getMethods())if(!m.getName().equals("toString")&&m.getReturnType()==r&&Arrays.equals(m.getParameterTypes(),p)) {
            m.setAccessible(true);
            return m;
        }
        throw new IllegalArgumentException(c+" method "+r+Arrays.toString(p));
    }
    static String err(Throwable e) {
        while(e.getCause()!=null)e=e.getCause();
        return e.getClass().getSimpleName()+":"+e.getMessage();
    }
    static void values(String label,String... names) {
        try {
            Class<?> c=cls(names);
            List<String> a=new ArrayList<>();
            for(Object o:c.getEnumConstants())a.add(((Enum<?>)o).ordinal()+":"+((Enum<?>)o).name());
            System.out.println(label+"="+a);
            for(Field f:c.getDeclaredFields())if(Modifier.isStatic(f.getModifiers())&&IntFunction.class.isAssignableFrom(f.getType())) {
                f.setAccessible(true);
                IntFunction<?> fun=(IntFunction<?>)f.get(null);
                for(int i:new int[] {
                    Integer.MIN_VALUE,-1,0,3,15,18,19,21,22,100,Integer.MAX_VALUE
                }
                )System.out.println(label+"_by_id["+i+"]="+fun.apply(i));
            }
        }
        catch(Exception e) {
            System.out.println(label+"="+err(e));
        }
    }
    static byte[] utf(String s)throws Exception {
        ByteArrayOutputStream out=new ByteArrayOutputStream();
        byte[] b=s.getBytes(StandardCharsets.UTF_8);
        int v=b.length;
        while((v&~127)!=0) {
            out.write((v&127)|128);
            v>>>=7;
        }
        out.write(v);
        out.write(b);
        return out.toByteArray();
    }
    static void utf()throws Exception {
        Class<?> b=cls("net.minecraft.network.FriendlyByteBuf","net.minecraft.network.PacketDataSerializer"), bb=cls("io.netty.buffer.ByteBuf"),un=cls("io.netty.buffer.Unpooled");
        Constructor<?> ctor=b.getConstructor(bb);
        Method wrap=un.getMethod("wrappedBuffer",byte[].class),empty=un.getMethod("buffer");
        Method read=exact(b,String.class),write=exact(b,b,String.class);
        for(int n:new int[] {
            16,17,40,41,32767,32768
        }
        ) {
            String s="x".repeat(n);
            Object in=ctor.newInstance(wrap.invoke(null,(Object)utf(s)));
            try {
                System.out.println("utf_read_"+n+"="+((String)read.invoke(in)).length());
            }
            catch(Exception e) {
                System.out.println("utf_read_"+n+"="+err(e));
            }
            Object out=ctor.newInstance(empty.invoke(null));
            try {
                write.invoke(out,s);
                System.out.println("utf_write_"+n+"=ok");
            }
            catch(Exception e) {
                System.out.println("utf_write_"+n+"="+err(e));
            }
        }
    }
    public static void main(String[] args)throws Exception {
        values("formatting","net.minecraft.ChatFormatting","net.minecraft.EnumChatFormat");
        values("display_slot","net.minecraft.world.scores.DisplaySlot");
        values("visibility","net.minecraft.world.scores.Team$Visibility","net.minecraft.world.scores.ScoreboardTeamBase$EnumNameTagVisibility");
        values("collision","net.minecraft.world.scores.Team$CollisionRule","net.minecraft.world.scores.ScoreboardTeamBase$EnumTeamPush");
        values("color","net.minecraft.world.scores.TeamColor");
        values("render_type","net.minecraft.world.scores.criteria.ObjectiveCriteria$RenderType","net.minecraft.world.scores.criteria.IScoreboardCriteria$EnumScoreboardHealthDisplay");
        utf();
    }
}
