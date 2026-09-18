//===============================================================================
//  #37 Cassandra Vablatsky
//===============================================================================

class  WCVablatsky extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardVablatskyTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Vablatskysmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardVablatskyBigTexture FILE=TEXTURES\menu\Folio\Cards\Vablatskybig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Cassandra Vablatsky";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=37
    skin(0)=Texture'HProps.skins.WizardCardVablatskyTex0'
	textureBig=Texture'WizCardVablatskyBigTexture'
	strDescriptionId="WizCard_0033"
}
